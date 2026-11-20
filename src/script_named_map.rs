//! Live read-only NamedNodeMap slice over the element's real attribute list.
//! SameObject handles need no second attribute cache. Named property reflection,
//! complete own-name enumeration and map mutators remain explicit unsupported
//! operations; indexed reads and named methods always return real stable Attrs.
use super::*;

const BASE: u32 = 0xc000_0000;
const INERT_BASE:u32=0xc100_0000;
pub(super) fn element(handle:u32)->Option<u32>{
    if (BASE..BASE+NODE_HANDLES).contains(&handle){Some(node_handle((handle-BASE) as usize))}
    else if (INERT_BASE+1..INERT_BASE+(script_inert_documents::ENTRY_END-script_inert_documents::MAIN_IMPLEMENTATION)).contains(&handle){Some(script_inert_documents::MAIN_IMPLEMENTATION+(handle-INERT_BASE))}
    else{None}
}
pub(super) fn handle(element:u32)->u32 {
    if node_index(element).is_some(){BASE+(element-NODE_BASE)}else{INERT_BASE+(element-script_inert_documents::MAIN_IMPLEMENTATION)}
}
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
        element(object).is_some_and(|owner| self.inert_node_type(owner)==Some(1)||self.class_list_element_receiver(owner))
    }
    fn named_map_owner(&mut self, object: u32) -> Result<u32, String> {
        let owner = element(object).ok_or("Invalid NamedNodeMap handle.")?;
        if self.inert_node_type(owner)==Some(1){return Ok(owner);}
        self.attr_element_node(owner).map(node_handle)
    }
    pub(super) fn named_map_get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        let owner = self.named_map_owner(object)?;
        self.state.work(1)?;
        let attrs=self.element_attribute_source(owner).ok_or("NamedNodeMap owner attributes are unavailable.")?;
        if property == "length" { return Ok(Value::Number(attrs.len() as f64)); }
        if let Some(index) = collection_index(property) {
            let slot = (index < attrs.len()).then_some(index);
            let value = self.lookup_owner_attribute_node_at(owner, slot)?;
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
        let attrs=self.element_attribute_source(owner).ok_or("NamedNodeMap owner attributes are unavailable.")?;
        let work=attrs.iter().fold(attrs.len(),|work,attr|work.saturating_add(attr.name.qualified.len()));self.state.work(work)?;let present=self.element_attribute_source(owner).ok_or("NamedNodeMap owner attributes are unavailable.")?.iter().any(|attr|attr.name.qualified==property);
        if present {
            return Err("NamedNodeMap named-property reflection is not implemented; use getNamedItem.".into());
        }
        Ok(Value::Undefined)
    }
    pub(super) fn named_map_has(&mut self, object: u32, property: &str) -> Result<bool, String> {
        if property == "length" || method(property).is_some() {
            self.named_map_owner(object)?;
            return Ok(true);
        }
        if let Some(index) = collection_index(property) {
            let owner = self.named_map_owner(object)?;
            self.state.work(1)?;
            let attrs=self.element_attribute_source(owner).ok_or("NamedNodeMap owner attributes are unavailable.")?;
            return Ok(index < attrs.len());
        }
        self.named_map_get(object, property).map(|value| !matches!(value, Value::Undefined))
    }
    pub(super) fn named_map_call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        let owner = self.named_map_owner(object)?;
        match method {
            "item" => {
                let index = timer_integer(args.first().ok_or("item requires an index.")?)? as u32 as usize;
                let attrs=self.element_attribute_source(owner).ok_or("NamedNodeMap owner attributes are unavailable.")?;
                self.lookup_owner_attribute_node_at(owner, (index < attrs.len()).then_some(index))
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
        if prototype_member(property) { self.named_map_owner(object)?; return Ok(None); }
        self.named_map_get(object, property)?;
        Ok(None)
    }
}

pub(super) fn operation(method:&str)->Option<&'static str>{match method{
    "\0NamedNodeMap.get.length"=>Some("length"),"\0NamedNodeMap.item"=>Some("item"),
    "\0NamedNodeMap.getNamedItem"=>Some("getNamedItem"),"\0NamedNodeMap.getNamedItemNS"=>Some("getNamedItemNS"),_=>None}}
pub(super) fn prototype_member(property:&str)->bool{matches!(property,"constructor"|"__proto__"|"length"|"item"|"getNamedItem"|"getNamedItemNS")}
