//! Live read-only NamedNodeMap slice over the element's real attribute list.
//! SameObject handles need no second attribute cache. Named property reflection,
//! complete own-name enumeration and map mutators remain explicit unsupported
//! operations; indexed reads and named methods always return real stable Attrs.
use super::*;

const BASE: u32 = 0xc000_0000;
pub(super) fn element(handle: u32) -> Option<u32> {
    (BASE..BASE + NODE_HANDLES).contains(&handle).then(|| node_handle((handle - BASE) as usize))
        .filter(|handle| node_index(*handle).is_some())
}
pub(super) fn handle(element: u32) -> u32 { BASE + (element - NODE_BASE) }
pub(super) fn method(name: &str) -> Option<&'static str> {
    match name {
        "item" => Some("item"),
        "getNamedItem" => Some("getNamedItem"),
        "getNamedItemNS" => Some("getNamedItemNS"),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn named_map_receiver(&self, object: u32) -> bool {
        element(object).is_some_and(|owner| self.class_list_element_receiver(owner))
    }
    fn named_map_node(&mut self, object: u32) -> Result<usize, String> {
        let owner = element(object).ok_or("Invalid NamedNodeMap handle.")?;
        self.attr_element_node(owner)
    }
    pub(super) fn named_map_get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        let node = self.named_map_node(object)?;
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        self.state.work(1)?;
        if property == "length" { return Ok(Value::Number(attrs.len() as f64)); }
        if let Some(index) = collection_index(property) {
            let slot = (index < attrs.len()).then_some(index);
            let value = self.lookup_attribute_node_at(node, slot)?;
            return Ok(if matches!(value, Value::Null) { Value::Undefined } else { value });
        }
        if matches!(property, "setNamedItem" | "setNamedItemNS" | "removeNamedItem" | "removeNamedItemNS") {
            return Err(format!("NamedNodeMap mutation {property:?} is not implemented."));
        }
        if matches!(property, "constructor" | "__proto__" | "prototype") {
            return Err("NamedNodeMap interface constructor/prototype access is not implemented.".into());
        }
        // Prototype members take precedence over named properties. This first
        // slice does not claim the named-property visibility/descriptor exotic.
        if matches!(property, "toString" | "toLocaleString" | "valueOf" | "hasOwnProperty" | "isPrototypeOf" | "propertyIsEnumerable" | "__defineGetter__" | "__defineSetter__" | "__lookupGetter__" | "__lookupSetter__") {
            return Ok(Value::Undefined);
        }
        if self.raw_attribute_slot(node, property)?.is_some() {
            return Err("NamedNodeMap named-property reflection is not implemented; use getNamedItem.".into());
        }
        Ok(Value::Undefined)
    }
    pub(super) fn named_map_has(&mut self, object: u32, property: &str) -> Result<bool, String> {
        if property == "length" || method(property).is_some() {
            self.named_map_node(object)?;
            return Ok(true);
        }
        if let Some(index) = collection_index(property) {
            let node = self.named_map_node(object)?;
            self.state.work(1)?;
            let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
            return Ok(index < attrs.len());
        }
        self.named_map_get(object, property).map(|value| !matches!(value, Value::Undefined))
    }
    pub(super) fn named_map_call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        let node = self.named_map_node(object)?;
        let owner = node_handle(node);
        match method {
            "item" => {
                let index = timer_integer(args.first().ok_or("item requires an index.")?)? as u32 as usize;
                let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
                self.lookup_attribute_node_at(node, (index < attrs.len()).then_some(index))
            }
            "getNamedItem" => self.get_attribute_node(owner, args),
            "getNamedItemNS" => self.get_attribute_node_ns(owner, args),
            _ => Err("NamedNodeMap operation is not implemented.".into()),
        }
    }
    pub(super) fn named_map_own_data(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if collection_index(property).is_some() {
            return self.named_map_get(object, property).map(|value| (!matches!(value, Value::Undefined)).then_some(value));
        }
        if property == "length" || method(property).is_some() { self.named_map_node(object)?; return Ok(None); }
        self.named_map_get(object, property)?;
        Ok(None)
    }
}
